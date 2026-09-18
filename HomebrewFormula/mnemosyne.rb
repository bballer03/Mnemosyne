class Mnemosyne < Formula
  desc "AI-powered JVM heap analysis tool — parse HPROF dumps, detect memory leaks, trace GC paths"
  homepage "https://github.com/bballer03/mnemosyne"
  version "0.6.1"
  license "Apache-2.0"

  if Hardware::CPU.arm?
    url "https://github.com/bballer03/mnemosyne/releases/download/v#{version}/mnemosyne-cli-aarch64-apple-darwin.tar.gz"
    sha256 "3ac1e4067db856bb3b6b0efa70a3cb1ac38af0c37cb3077b4f6156a50d0d0113"
  else
    url "https://github.com/bballer03/mnemosyne/releases/download/v#{version}/mnemosyne-cli-x86_64-apple-darwin.tar.gz"
    sha256 "120e28d43a576d2c7eb043b45a32461714d8b891f87aa0c630b24ca3fa3a2a08"
  end

  def install
    bin.install "mnemosyne-cli"
  end

  test do
    assert_match "mnemosyne-cli", shell_output("#{bin}/mnemosyne-cli --version")
  end
end
