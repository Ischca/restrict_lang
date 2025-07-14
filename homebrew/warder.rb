class Warder < Formula
  desc "Package manager for Restrict Language"
  homepage "https://restrict-lang.org"
  url "https://github.com/restrict-lang/restrict_lang/archive/v0.1.0.tar.gz"
  sha256 "PLACEHOLDER_SHA256"
  license "MIT"
  head "https://github.com/restrict-lang/restrict_lang.git", branch: "main"

  depends_on "rust" => :build

  def install
    # Build the main compiler
    system "cargo", "build", "--release", "--locked"
    bin.install "target/release/restrict_lang"

    # Build warder
    cd "warder" do
      system "cargo", "build", "--release", "--locked"
      bin.install "target/release/warder"
    end

    # Install shell completions
    bash_completion.install "completions/warder.bash" => "warder"
    fish_completion.install "completions/warder.fish"
    zsh_completion.install "completions/_warder"

    # Install man pages
    man1.install Dir["docs/man/*.1"]
  end

  test do
    # Test warder version
    assert_match "warder", shell_output("#{bin}/warder --version")
    
    # Test creating a new project
    system "#{bin}/warder", "new", "test_project"
    assert_predicate testpath/"test_project/package.rl.toml", :exist?
    
    # Test restrict_lang compiler
    (testpath/"hello.rl").write <<~EOS
      fn main() {
        "Hello, World!" |> println
      }
    EOS
    
    system "#{bin}/restrict_lang", "compile", "hello.rl"
  end
end