if FileTest.exist?(path) || condition
  FileUtils.mkdir(path)
end
