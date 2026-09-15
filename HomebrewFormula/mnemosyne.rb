class Mnemosyne < Formula
  desc "AI-powered JVM heap analysis tool — parse HPROF dumps, detect memory leaks, trace GC paths"
  homepage "https://github.com/bballer03/mnemosyne"
  version "0.5.0"
  license "Apache-2.0"

  if Hardware::CPU.arm?
    url "https://github.com/bballer03/mnemosyne/releases/download/v#{version}/mnemosyne-cli-aarch64-apple-darwin.tar.gz"
    sha256 "0bb86cd5d76a35e78d903494c796616a34b0d0ace1f2a39256b47e491e109033"
  else
    url "https://github.com/bballer03/mnemosyne/releases/download/v#{version}/mnemosyne-cli-x86_64-apple-darwin.tar.gz"
    sha256 "1262fc9f18ecfe47fd7b61a5982aab7494f806be695ee9864e5e93c5dc864247"
  end

  def install
    bin.install "mnemosyne-cli"
  end

  test do
    assert_match "mnemosyne-cli", shell_output("#{bin}/mnemosyne-cli --version")
  end
end
