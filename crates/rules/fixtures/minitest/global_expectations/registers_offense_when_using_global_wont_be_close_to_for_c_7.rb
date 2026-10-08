it 'does something' do
  options[:a][:b].wont_be_close_to 0
  ^^^^^^^^^^^^^^^ Use `value(options[:a][:b])` instead.
end
