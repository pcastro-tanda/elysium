unless FileTest.exists?(path)
^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Remove unnecessary existence check `FileTest.exists?`.
  FileUtils.makedirs(path)
end
