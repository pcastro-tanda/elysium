if FileTest.exist?(path)
^^^^^^^^^^^^^^^^^^^^^^^^ Remove unnecessary existence check `FileTest.exist?`.
  FileUtils.unlink(path)
  ^^^^^^^^^^^^^^^^^^^^^^ Use atomic file operation method `FileUtils.rm_f`.
end
