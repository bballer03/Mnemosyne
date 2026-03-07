class Mnemosyne < Formula
  desc "AI-powered JVM heap analysis tool — parse HPROF dumps, detect memory leaks, trace GC paths"
  homepage "https://github.com/bballer03/mnemosyne"
  version "0.1.0"
  license "Apache-2.0"

  if Hardware::CPU.arm?
    url "https://github.com/bballer03/mnemosyne/releases/download/v#{version}/mnemosyne-cli-aarch64-apple-darwin.tar.gz"
    sha256 "bd4a98e41ff4982d7ecb0c2ed7cdfc85eab78b1a0d0f0abc801b9c96a404609e"
  else
    url "https://github.com/bballer03/mnemosyne/releases/download/v#{version}/mnemosyne-cli-x86_64-apple-darwin.tar.gz"
    sha256 "4f5c6fdae403269bdb1cb22ce117c5e7491d24bc152cb76bdadb53f01199cea0"
  end

  def install
    bin.install "mnemosyne-cli"
  end

  test do
    assert_match "mnemosyne-cli", shell_output("#{bin}/mnemosyne-cli --version")
  end
end