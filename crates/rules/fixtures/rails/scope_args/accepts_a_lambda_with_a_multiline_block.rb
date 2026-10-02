scope :active, (lambda do |active|
                 where(active: active)
               end)
