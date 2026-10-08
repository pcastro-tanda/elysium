array.delete_if(&:nil?)
array.delete_if { |e| e.nil? }
