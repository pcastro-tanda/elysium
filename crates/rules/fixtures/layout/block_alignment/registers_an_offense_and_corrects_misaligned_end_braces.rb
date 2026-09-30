def get_gems_by_name
  @gems ||= Hash[*get_latest_gems.map { |gem|
                   [gem.name, gem, gem.full_name, gem]
              }.flatten]
              ^ `}` at 4, 14 is not aligned with `*get_latest_gems.map { |gem|` at 2, 17 or `@gems ||= Hash[*get_latest_gems.map { |gem|` at 2, 2.
end
