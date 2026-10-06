FileUtils.chdir("/var/run") do
^^^^^^^^^^^^^^^^^^^^^^^^^^^ Avoid using `FileUtils.chdir` due to its process-wide effect.
  puts Dir.pwd
end
