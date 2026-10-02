if FileTest.exist?(path)
^^^^^^^^^^^^^^^^^^^^^^^^ Remove unnecessary existence check `FileTest.exist?`.
  FileUtils.remove_entry_secure(path)
  ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Use atomic file operation method `FileUtils.rm_rf`.
end
