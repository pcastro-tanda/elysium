names.each do |name|
  process(name)
  Rails.logger.debug { "The name is #{name}" }
end
