Dir["./lib/**/*.rb"].sort.each do |file|
  require file
end
