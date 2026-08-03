class Warder < Formula
  desc "Compiler and project tool for Restrict Language"
  homepage "https://ischca.github.io/restrict_lang/"
  head "https://github.com/Ischca/restrict_lang.git", branch: "main"
  license "MIT"

  depends_on "rust" => :build

  def install
    system "cargo", "build", "--workspace", "--release", "--locked"
    bin.install "target/release/restrict_lang"
    bin.install "target/release/warder"
  end

  test do
    assert_match "restrict_lang", shell_output("#{bin}/restrict_lang --version")
    assert_match "warder", shell_output("#{bin}/warder --version")

    (testpath/"hello.rl").write <<~RESTRICT
      fun main: () -> Int32 = {
          42
      }
    RESTRICT

    system bin/"restrict_lang", "hello.rl", "hello.wat"
    assert_predicate testpath/"hello.wat", :exist?
  end
end
