def change_dir(&block)
  Dir&.chdir("/var/run", &block)
  ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Avoid using `Dir&.chdir` due to its process-wide effect.
end
