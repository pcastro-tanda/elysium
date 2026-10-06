Dir&.chdir("/var/run") do |dir|
^^^^^^^^^^^^^^^^^^^^^^ Avoid using `Dir&.chdir` due to its process-wide effect.
  puts dir
end
