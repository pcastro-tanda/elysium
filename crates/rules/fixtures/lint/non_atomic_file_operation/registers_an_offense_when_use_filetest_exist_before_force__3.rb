unless FileTest.exist?(path)
^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Remove unnecessary existence check `FileTest.exist?`.
  FileUtils.mkpath(path)
end
