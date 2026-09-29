map { |a|
  begin
do_something
ensure
  puts 'oh no'
end
}.join('-')
