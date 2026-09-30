KeyMap = Hash.new do |map, key|
  value = if key.respond_to?(:to_str)
    key
  else
    key.to_s.split('_').
      each { |w| w.capitalize! }.
      join('-')
  end
  keymap_mutex.synchronize { map[key] = value }
end
