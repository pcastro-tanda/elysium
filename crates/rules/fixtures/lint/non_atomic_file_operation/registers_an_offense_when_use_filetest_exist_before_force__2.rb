unless FileTest.exist?(path)
^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Remove unnecessary existence check `FileTest.exist?`.
  FileUtils.mkdir_p(path)
end
