def investigate(processed_source)
  @modifier = processed_source
              .tokens
              .select { |t| t.type == :k }
              .map(&:pos)
end
