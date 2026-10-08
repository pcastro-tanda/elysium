if FileTest.exist?(path) && File.stat(path).socket?
  FileUtils.mkdir(path)
end
