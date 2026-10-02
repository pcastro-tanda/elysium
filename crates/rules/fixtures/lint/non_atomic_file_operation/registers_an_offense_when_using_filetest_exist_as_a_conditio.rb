if condition
  do_something
elsif FileTest.exist?(path)
^^^^^^^^^^^^^^^^^^^^^^^^^^^ Remove unnecessary existence check `FileTest.exist?`.
  FileUtils.rm_f path
end
