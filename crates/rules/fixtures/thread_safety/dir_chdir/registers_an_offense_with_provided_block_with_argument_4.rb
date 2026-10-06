FileUtils.cd("/var/run") do |dir|
^^^^^^^^^^^^^^^^^^^^^^^^ Avoid using `FileUtils.cd` due to its process-wide effect.
  puts dir
end
