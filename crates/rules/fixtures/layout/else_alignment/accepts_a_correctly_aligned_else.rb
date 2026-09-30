begin
  raise StandardError.new('Fail') if rand(2).odd?
rescue StandardError => error
  $stderr.puts error.message
else
  $stdout.puts 'Lucky you!'
end
