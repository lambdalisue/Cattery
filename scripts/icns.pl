#!/usr/bin/env perl
# Pack PNGs into an Apple icon file, which macOS reads PNG entries from
# whole: icns.pl OUT.icns TYPE=FILE.png ..., with TYPE an entry such as
# ic07 (128px), ic08 (256px), ic09 (512px) or ic10 (1024px).
use strict;
use warnings;

my ($out, @entries) = @ARGV;
die "usage: icns.pl OUT.icns TYPE=FILE.png ...\n" unless $out && @entries;
my $body = '';
for my $entry (@entries) {
    my ($type, $file) = split /=/, $entry, 2;
    die "an entry type is four letters: $type\n" unless length($type) == 4;
    open my $in, '<:raw', $file or die "$file: $!\n";
    my $png = do { local $/; <$in> };
    close $in;
    die "$file is not a PNG\n" unless substr($png, 0, 8) eq "\x89PNG\r\n\x1a\n";
    $body .= $type . pack('N', length($png) + 8) . $png;
}
open my $fh, '>:raw', $out or die "$out: $!\n";
print $fh 'icns' . pack('N', length($body) + 8) . $body;
close $fh;
