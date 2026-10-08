if FileTest.exist?(path)
  FileUtils.rm_r(path)
end
