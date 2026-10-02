if !FileTest.exist?(path)
^^^^^^^^^^^^^^^^^^^^^^^^^ Remove unnecessary existence check `FileTest.exist?`.
  FileUtils.makedirs(path)
end
