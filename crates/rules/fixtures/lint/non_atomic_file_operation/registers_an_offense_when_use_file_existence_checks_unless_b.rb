FileUtils.mkdir(path) unless FileTest.exist?(path)
                      ^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Remove unnecessary existence check `FileTest.exist?`.
^^^^^^^^^^^^^^^^^^^^^ Use atomic file operation method `FileUtils.mkdir_p`.
