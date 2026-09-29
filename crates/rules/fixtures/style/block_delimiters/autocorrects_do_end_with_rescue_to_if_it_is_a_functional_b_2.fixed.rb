map { |a|
  begin
do_something
rescue StandardError => e
  puts 'oh no'
end
}.join('-')
