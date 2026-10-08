it 'does something' do
  options[:a][:b].wont_be_same_as 0
  ^^^^^^^^^^^^^^^ Use `expect(options[:a][:b])` instead.
end
