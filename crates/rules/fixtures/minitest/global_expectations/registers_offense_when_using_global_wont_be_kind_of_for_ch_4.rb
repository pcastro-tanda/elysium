it 'does something' do
  options[:a][:b].wont_be_kind_of 0
  ^^^^^^^^^^^^^^^ Use `_(options[:a][:b])` instead.
end
