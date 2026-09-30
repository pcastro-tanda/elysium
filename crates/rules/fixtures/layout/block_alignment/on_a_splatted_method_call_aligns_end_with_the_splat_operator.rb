def get_gems_by_name
  @gems ||= Hash[*get_latest_gems.map { |gem|
                   [gem.name, gem, gem.full_name, gem]
                 }.flatten]
end
