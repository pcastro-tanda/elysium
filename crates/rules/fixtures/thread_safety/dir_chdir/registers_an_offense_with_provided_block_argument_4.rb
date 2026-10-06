def change_dir(&block)
  FileUtils.cd("/var/run", &block)
  ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Avoid using `FileUtils.cd` due to its process-wide effect.
end
