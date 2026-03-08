class Mnemosyne < Formula
  desc "AI-powered JVM heap analysis tool — parse HPROF dumps, detect memory leaks, trace GC paths"
  homepage "https://github.com/bballer03/mnemosyne"
  version "0.2.0"
  license "Apache-2.0"

  if Hardware::CPU.arm?
    url "https://github.com/bballer03/mnemosyne/releases/download/v#{version}/mnemosyne-cli-aarch64-apple-darwin.tar.gz"
    sha256 "54cd280bcd55901ac84faa201605e70288124a0a8141e14a86deea54fbbe46d2"
  else
    url "https://github.com/bballer03/mnemosyne/releases/download/v#{version}/mnemosyne-cli-x86_64-apple-darwin.tar.gz"
    sha256 "d60334546086d2254633908f57249df981c16da474b501069cf3e619b3b27c5a"
  end

  def install
    bin.install "mnemosyne-cli"
  end

  test do
    assert_match "mnemosyne-cli", shell_output("#{bin}/mnemosyne-cli --version")
  end
end