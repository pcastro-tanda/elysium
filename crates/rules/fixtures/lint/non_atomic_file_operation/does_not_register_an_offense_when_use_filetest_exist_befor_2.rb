if FileTest.exist?(path)
  FileUtils.rmtree(path)
end
