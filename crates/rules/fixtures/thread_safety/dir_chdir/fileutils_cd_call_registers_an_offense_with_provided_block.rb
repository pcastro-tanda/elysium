FileUtils.cd("/var/run") do
^^^^^^^^^^^^^^^^^^^^^^^^ Avoid using `FileUtils.cd` due to its process-wide effect.
  puts Dir.pwd
end
