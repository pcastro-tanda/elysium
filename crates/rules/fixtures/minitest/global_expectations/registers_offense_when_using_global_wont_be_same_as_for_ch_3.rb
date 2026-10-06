it 'does something' do
  options[:a][:b].wont_be_same_as 0
  ^^^^^^^^^^^^^^^ Use `_(options[:a][:b])` instead.
end
