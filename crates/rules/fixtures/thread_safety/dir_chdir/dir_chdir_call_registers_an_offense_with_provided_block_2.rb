Dir&.chdir("/var/run") do
^^^^^^^^^^^^^^^^^^^^^^ Avoid using `Dir&.chdir` due to its process-wide effect.
  puts Dir.pwd
end
