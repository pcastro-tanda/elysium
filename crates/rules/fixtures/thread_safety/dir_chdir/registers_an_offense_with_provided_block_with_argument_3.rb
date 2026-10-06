FileUtils.chdir("/var/run") do |dir|
^^^^^^^^^^^^^^^^^^^^^^^^^^^ Avoid using `FileUtils.chdir` due to its process-wide effect.
  puts dir
end
