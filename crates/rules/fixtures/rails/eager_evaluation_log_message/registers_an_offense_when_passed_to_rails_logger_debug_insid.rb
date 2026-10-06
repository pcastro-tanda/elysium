names.each do |name|
  process(name)
  Rails.logger.debug "The name is #{name}"
                     ^^^^^^^^^^^^^^^^^^^^^ Pass a block to `Rails.logger.debug`.
end
