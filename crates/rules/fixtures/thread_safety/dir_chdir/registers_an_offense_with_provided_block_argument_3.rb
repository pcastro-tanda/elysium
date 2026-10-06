def change_dir(&block)
  FileUtils.chdir("/var/run", &block)
  ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Avoid using `FileUtils.chdir` due to its process-wide effect.
end
