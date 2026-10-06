it 'does something' do
  options[:a][:b].wont_be_within_delta 0
  ^^^^^^^^^^^^^^^ Use `_(options[:a][:b])` instead.
end
