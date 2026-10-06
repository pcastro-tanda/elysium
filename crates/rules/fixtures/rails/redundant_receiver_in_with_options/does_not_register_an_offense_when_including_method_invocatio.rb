client = ApplicationClient.new
with_options options: false do |merger|
  client.invoke(merger.something, something)
end
