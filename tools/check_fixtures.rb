#!/usr/bin/env ruby
# frozen_string_literal: true

# Development/CI tool only. Verifies that every fixture directory under
# crates/rules/fixtures is exactly what tools/port_spec.rb generates from the
# pinned RuboCop source, minus the cases listed in
# crates/rules/fixtures/REMOVED.txt (each explained in the fixtures README).
# A stale fixture set (e.g. generated from an older RuboCop) fails the check.
#
#   ruby tools/check_fixtures.rb --rubocop-src ../corpus/rubocop-1.91.0 [--jobs 8] [--write]
#
# --write replaces each fixture directory with the regenerated one (minus
# REMOVED.txt cases) instead of failing: the re-baseline mode.

require 'fileutils'
require 'optparse'
require 'open3'
require 'set'
require 'tmpdir'

ROOT = File.expand_path('..', __dir__)
options = { fixtures: File.join(ROOT, 'crates/rules/fixtures'), jobs: 4, write: false }
OptionParser.new do |opts|
  opts.on('--rubocop-src PATH', 'RuboCop source checkout (lib/ and spec/)') { |v| options[:src] = File.expand_path(v) }
  opts.on('--fixtures DIR') { |v| options[:fixtures] = File.expand_path(v) }
  opts.on('--jobs N', Integer) { |v| options[:jobs] = v }
  opts.on('--write', 'Rewrite fixture dirs instead of failing') { options[:write] = true }
  opts.on('--only DIR', 'Check only this dept/cop dir (repeatable)') { |v| (options[:only] ||= []) << v }
end.parse!
abort 'missing --rubocop-src' unless options[:src]

fixtures = options[:fixtures]
removed_path = File.join(fixtures, 'REMOVED.txt')
removed = File.readlines(removed_path, chomp: true).reject { |l| l.empty? || l.start_with?('#') }.to_set
removed_by_dir = removed.group_by { |c| File.dirname(c) }

# "dept/snake" fixture dir -> "Dept/CopName", read from the cop's own source.
def cop_name(src, dir)
  path = File.join(src, 'lib/rubocop/cop', "#{dir}.rb")
  return nil unless File.exist?(path)

  text = File.read(path)
  dept = text[/^\s*module RuboCop\s*\n\s*module Cop\s*\n\s*module (\w+)/, 1]
  klass = text[/^      class (\w+) < /, 1]
  dept && klass ? "#{dept}/#{klass}" : nil
end

# Case name of a fixture file: everything before the first '.' (`foo.fixed.rb`,
# `foo.yml`, `foo.nofix`, ...). Hidden harness temp files are never fixtures.
def case_files(dir)
  return {} unless Dir.exist?(dir)

  Dir.children(dir).reject { |f| f.start_with?('.') }.group_by { |f| f.split('.').first }
end

dirs = Dir.glob(File.join(fixtures, '*/*/')).map { |d| d.delete_prefix("#{fixtures}/").chomp('/') }.sort
dirs &= options[:only] if options[:only]
work = Dir.mktmpdir('check_fixtures_')
problems = []
queue = Queue.new
dirs.each { |d| queue << d }
mutex = Mutex.new

workers = Array.new(options[:jobs]) do
  Thread.new do
    while (dir = begin queue.pop(true) rescue nil end)
      cop = cop_name(options[:src], dir)
      unless cop
        mutex.synchronize { problems << "#{dir}: no RuboCop cop source at lib/rubocop/cop/#{dir}.rb" }
        next
      end
      out = File.join(work, dir.tr('/', '__'))
      log, status = Open3.capture2e('ruby', File.join(ROOT, 'tools/port_spec.rb'), '--cop', cop,
                                    '--rubocop-src', options[:src], '--out', out)
      unless status.success?
        mutex.synchronize { problems << "#{dir}: port_spec failed\n#{log.lines.last(5).join}" }
        next
      end
      gen_dir = File.join(out, dir)
      gen = case_files(gen_dir)
      diffs = []
      (removed_by_dir[dir] || []).each do |c|
        next if gen.delete(File.basename(c))

        diffs << "  REMOVED.txt lists #{File.basename(c)}, which the spec no longer has"
      end
      have = case_files(File.join(fixtures, dir))

      if options[:write]
        target = File.join(fixtures, dir)
        FileUtils.rm_rf(target)
        FileUtils.mkdir_p(target)
        gen.each_value { |files| files.each { |f| FileUtils.cp(File.join(gen_dir, f), target) } }
        next
      end

      (gen.keys - have.keys).each { |c| diffs << "  missing case #{c}" }
      (have.keys - gen.keys).each { |c| diffs << "  unexpected case #{c} (not in the spec; add to REMOVED.txt only if deliberately removed)" }
      (gen.keys & have.keys).each do |c|
        if gen[c].sort != have[c].sort
          diffs << "  #{c}: files differ (#{gen[c].sort.join(' ')} vs #{have[c].sort.join(' ')})"
        elsif gen[c].any? { |f| !FileUtils.compare_file(File.join(gen_dir, f), File.join(fixtures, dir, f)) }
          diffs << "  #{c}: content differs"
        end
      end
      mutex.synchronize { problems << "#{dir}:\n#{diffs.join("\n")}" } unless diffs.empty?
    end
  end
end
workers.each(&:join)
FileUtils.remove_entry(work)

stale = removed.reject { |c| Dir.exist?(File.join(fixtures, File.dirname(c))) }
problems << "REMOVED.txt entries for unknown fixture dirs:\n  #{stale.join("\n  ")}" unless stale.empty?

if problems.empty?
  puts "fixtures match #{File.basename(options[:src])} for #{dirs.size} cops"
else
  puts problems.sort
  puts "\n#{problems.size} problem(s); regenerate with tools/port_spec.rb (or --write) and log removals in REMOVED.txt"
  exit 1
end
