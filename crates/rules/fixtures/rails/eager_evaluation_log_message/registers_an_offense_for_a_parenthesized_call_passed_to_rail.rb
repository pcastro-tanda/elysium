names.each do |name|
  Rails.logger.debug("The name is #{name}")
                    ^^^^^^^^^^^^^^^^^^^^^^^ Pass a block to `Rails.logger.debug`.
end
