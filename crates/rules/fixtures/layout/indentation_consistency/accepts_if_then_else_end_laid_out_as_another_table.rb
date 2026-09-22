if File.exist?('config.save')
then ConfigTable.load
else ConfigTable.new
end
