it 'does something' do
  options[:a][:b].wont_be_within_delta 0
  ^^^^^^^^^^^^^^^ Use `expect(options[:a][:b])` instead.
end
