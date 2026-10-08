it 'does something' do
  options[:a][:b].wont_respond_to 0
  ^^^^^^^^^^^^^^^ Use `expect(options[:a][:b])` instead.
end
