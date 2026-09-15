class Mnemosyne < Formula
  desc "AI-powered JVM heap analysis tool — parse HPROF dumps, detect memory leaks, trace GC paths"
  homepage "https://github.com/bballer03/mnemosyne"
  version "0.5.0"
  license "Apache-2.0"

  if Hardware::CPU.arm?
    url "https://github.com/bballer03/mnemosyne/releases/download/v#{version}/mnemosyne-cli-aarch64-apple-darwin.tar.gz"
    sha256 "8555fcb7965d2a2d5c9d3ffcca2a5ddc5d9f856e54ea97143c96bf9578733b5e"
  else
    url "https://github.com/bballer03/mnemosyne/releases/download/v#{version}/mnemosyne-cli-x86_64-apple-darwin.tar.gz"
    sha256 "e9d91d25faa5f3a7af0c482a6a48ab8e0e6295a43007e6c122bc602e1e079eb6"
  end

  def install
    bin.install "mnemosyne-cli"
  end

  test do
    assert_match "mnemosyne-cli", shell_output("#{bin}/mnemosyne-cli --version")
  end
end
