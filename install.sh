#!/usr/bin/env bash

set -euo pipefail

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Print colored output
print_info() {
    echo -e "${BLUE}[INFO]${NC} $1"
}

print_success() {
    echo -e "${GREEN}[SUCCESS]${NC} $1"
}

print_warning() {
    echo -e "${YELLOW}[WARNING]${NC} $1"
}

print_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

# Check if running in Nix environment
check_nix() {
    if ! command -v nix &> /dev/null; then
        print_error "Nix package manager not found!"
        print_info "Please install Nix from https://nixos.org/download.html"
        exit 1
    fi
    
    if ! nix --version | grep -q "flakes" 2>/dev/null; then
        print_warning "Nix flakes not enabled. Enabling flakes..."
        mkdir -p ~/.config/nix
        echo "experimental-features = nix-command flakes" >> ~/.config/nix/nix.conf
    fi
}

# Build the application
build_app() {
    print_info "Building DraVi..."
    if nix build; then
        print_success "Build completed successfully!"
    else
        print_error "Build failed!"
        exit 1
    fi
}

# Install the binary
install_binary() {
    local install_dir="$HOME/.local/bin"
    local binary_path="$PWD/result/bin/dravi"
    local target_path="$install_dir/dravi"
    
    print_info "Installing DraVi to $target_path..."
    
    # Create install directory if it doesn't exist
    if ! mkdir -p "$install_dir"; then
        print_error "Failed to create directory $install_dir"
        print_warning "Please manually copy $binary_path to a directory in your PATH"
        exit 1
    fi
    
    # Remove old binary if it exists
    if [[ -f "$target_path" ]]; then
        print_info "Removing old version..."
        rm -f "$target_path" 2>/dev/null || {
            print_warning "Could not remove old binary, trying sudo..."
            sudo rm -f "$target_path" 2>/dev/null || {
                print_error "Failed to remove old binary at $target_path"
                print_warning "Please manually remove it first: sudo rm $target_path"
                exit 1
            }
        }
    fi
    
    # Copy the binary with error handling
    if [[ -f "$binary_path" ]]; then
        if cp "$binary_path" "$target_path" 2>/dev/null; then
            if chmod +x "$target_path" 2>/dev/null; then
                print_success "DraVi installed to $target_path"
            else
                print_error "Failed to make binary executable"
                print_warning "Please run: chmod +x $target_path"
                exit 1
            fi
        else
            print_warning "Permission denied, trying with sudo..."
            if sudo cp "$binary_path" "$target_path" 2>/dev/null && sudo chmod +x "$target_path" 2>/dev/null; then
                print_success "DraVi installed to $target_path (with sudo)"
            else
                print_error "Failed to copy binary to $target_path"
                print_warning "Please manually copy $binary_path to $target_path"
                exit 1
            fi
        fi
    else
        print_error "Binary not found at $binary_path"
        print_info "Make sure 'nix build' completed successfully"
        exit 1
    fi
}

# Update PATH
update_path() {
    local shell_profile=""
    local install_dir="$HOME/.local/bin"
    
    # Check if PATH already contains ~/.local/bin
    if [[ ":$PATH:" == *":$install_dir:"* ]]; then
        print_info "~/.local/bin is already in PATH"
        return
    fi
    
    # Detect shell and set appropriate profile file
    case "$SHELL" in
        */bash)
            shell_profile="$HOME/.bashrc"
            ;;
        */zsh)
            shell_profile="$HOME/.zshrc"
            ;;
        */fish)
            shell_profile="$HOME/.config/fish/config.fish"
            ;;
        *)
            shell_profile="$HOME/.profile"
            ;;
    esac
    
    # Create directory structure if needed
    if [[ "$SHELL" == */fish ]]; then
        mkdir -p "$(dirname "$shell_profile")"
    fi
    
    # Create profile file if it doesn't exist
    if [[ ! -f "$shell_profile" ]]; then
        print_info "Creating $shell_profile"
        touch "$shell_profile" || {
            print_error "Failed to create $shell_profile"
            print_warning "Please manually add $install_dir to your PATH"
            return
        }
    fi
    
    print_info "Adding ~/.local/bin to PATH in $shell_profile"
    
    # Add to PATH with error handling
    if [[ "$SHELL" == */fish ]]; then
        echo "set -gx PATH \$PATH $install_dir" >> "$shell_profile" 2>/dev/null || {
            print_error "Failed to write to $shell_profile"
            print_warning "Please manually add: set -gx PATH \$PATH $install_dir"
            return
        }
    else
        echo "export PATH=\"\$PATH:$install_dir\"" >> "$shell_profile" 2>/dev/null || {
            print_error "Failed to write to $shell_profile"
            print_warning "Please manually add: export PATH=\"\$PATH:$install_dir\""
            return
        }
    fi
    
    print_success "Added ~/.local/bin to PATH"
    print_warning "Please restart your shell or run: source $shell_profile"
}

# Check dependencies
check_dependencies() {
    print_info "Checking dependencies..."
    
    local missing_deps=()
    
    # Check for typst
    if ! command -v typst &> /dev/null; then
        print_warning "typst not found - PDF compilation will not work"
        print_info "Install typst with: nix profile install nixpkgs#typst"
    fi
    
    # Check for tdf (terminal PDF viewer)
    if ! command -v tdf &> /dev/null; then
        print_warning "tdf not found - PDF rendering will not work"
        print_info "Install tdf with your package manager or build from source"
    fi
}

# Main installation function
main() {
    print_info "Starting DraVi installation..."
    
    check_nix
    check_dependencies
    
    if ! build_app; then
        print_error "Build failed. Please check the error messages above."
        exit 1
    fi
    
    # Try standard installation first
    if install_binary && update_path; then
        print_success "Installation completed!"
        print_info "You can now run 'dravi' from anywhere in your terminal"
        print_info "Make sure to restart your shell or source your profile to update PATH"
    else
        # Fallback installation instructions
        echo
        print_warning "Standard installation failed. Here are manual installation steps:"
        echo
        print_info "1. Copy the binary manually:"
        echo "   cp $PWD/result/bin/dravi ~/.local/bin/dravi"
        echo "   chmod +x ~/.local/bin/dravi"
        echo
        print_info "2. Add to PATH (add to your shell profile):"
        echo "   export PATH=\"\$PATH:\$HOME/.local/bin\""
        echo
        print_info "3. Or run directly from build directory:"
        echo "   $PWD/result/bin/dravi"
    fi
    
    echo
    print_info "Usage:"
    echo "  dravi                 - Start DraVi in current directory"
    echo
    print_info "In DraVi:"
    echo "  hjkl                  - Move cursor"
    echo "  i                     - Enter text mode"
    echo "  space                 - Draw/place character"
    echo "  s                     - Save as Typst file (drawing.typ)"
    echo "  r                     - View PDF with tdf"
    echo "  ?                     - Settings menu"
    echo "  q                     - Quit"
}

# Run installation
main "$@"