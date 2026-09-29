[].inject({}) { |h| h[rand] = rand; h }

[].reduce({}) do |h|
   h[rand] = rand
   h
end
