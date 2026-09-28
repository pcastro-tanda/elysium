Dir["./lib/**/*.rb"].each do |file|
  if file.start_with?('_')
    puts "Not required."
  else
    require file
  end
end
