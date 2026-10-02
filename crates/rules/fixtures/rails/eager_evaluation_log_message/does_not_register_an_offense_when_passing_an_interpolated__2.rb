Rails.logger.debug { "The time is #{Time.now}" }
Rails.logger.debug do
  "The time is #{Time.now}"
end
