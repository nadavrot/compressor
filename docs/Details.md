# Technical details

This page describes the techniques and the design decisions made in this project. 

# Overall structure

The compressor has a few components, that are described in detail below. This section describes the data-compression pipeline and how the pieces fit together. 
 
The input is split into large chunks that are compressed independently. The maximum size of each chunk is 4GB, to allow the use of 32bit indices within each chunk. Each chunk is wrapped with a header that includes a length a magic signature. Individual transformations can further split chunks into smaller chunks. 

The phase of compression is the matcher. The matcher is responsible for splitting the input stream into a sequence of packets that describe a region of literals, followed by a reference to a sequence of bytes from earlier in the file. Every packet returns the two sequences (literal region and match region), and either regions can be empty. The compressor can encode the match sequence into LZ4 encoding. This was implemented to allow a comparison to the LZ4 matcher. 

After the matcher extracts the match regions, the sequence of match and literal regions are decomposed into four streams:

1. Literals (a sequence of bytes as they appear in the input file).
2. The length of the literal segment.
3. The offset to the match.
4. The length of the match region. 

The four streams are handled in different ways. The literal region, and the two length buffers are each compressed independently with the entropy encoder. The buffer that holds the offsets, which is also the biggest of the four buffers, is handled separately. The offset buffer is first transformed to reduce the cost of repetitive offsets. The offsets are shifted to free the range zero to three. The values zero to three represent offset values that have appeared in the last three places in the stream. Repetitive offsets often show up in structured data. We don’t increase this value further because the probability of repetitive offsets drops very quickly, and increasing the length of offsets can be costly.

After transforming the offset stream, the stream is split into two buffers: extra-bits, and bit-length. The extra-bits buffer holds the raw bits of the offsets, while the other stream holds a byte that specifies the number of bits that were saved into the bit-stream. This is an effective form of compression because the distribution of offset lengths is very sharp and most offsets requite few bits to encode. The second buffer of 8-bit values that represent the length of the numbers in the extra-bit section are now compressed using entropy encoding. Entropy encoding is effective because of the sharp histogram in the offset-length values. Notice that we don’t save the upper bit of the binary number in the extra-bits buffer, because it always has to be equal to one, otherwise we would have made the number shorter.

Finally the four streams are concatenated together. It is possible to accelerate the encoding and decoding stream by interleaving the encoding of regions into multiple parallel streams but this is not currently implemented. 

# Matcher   

The matcher is responsible for iterating over the input and return a sequence of pairs: literal region and match region. The matcher is implemented as an iterator that allows the compressor to process the matches one at a time, and work on the input in chunks. The matcher has two parts: the dictionary or cache that finds the matches, and the parser that selects the matches. The project has two parsers. A traditional look-ahead parser and an optimal parser. 

## The dictionary

Both of the matchers rely on an underlying cache data structure that finds common sequences of bytes. The dictionary is built as a multi-way cache. The size of the cache and the number of ways is configured according to the compression level. On every byte the matcher reads the four consecutive bytes and creates a hash value that determines where in the cache the index of the byte will be stored. The ways in the LRU cache are rotated on each write. 

To find matches from earlier in the file the matcher fetches 4 bytes and finds the entry in the cache. If there are no cache collisions then each of the items in the way represent a match from earlier in the file. The matches are sorted by the distance from the current index. This allows the search procedure to stop early if we only allow distances of certain length. After a match is searched the index of the current location can be saved to the cache, as described above. 

There are a few tricks that can speed up the match speed. First, after a first match is found it is possible to quickly disqualify future matches. If a match of length X is found, the cache module starts looking at index X+1, to quickly disqualify the match, instead of scanning from the first byte of the match string. 

## Parser

THe parser is responsible for selecting the right matches between multiple candidates, while minimizing compression time. Selecting a match may preclude the possibility of selecting a longer match that may start one or two bytes later. It is not practical to scan all possible combinations, and the parser needs to rely on heuristics and skip some combinations. 

The parser uses a look-ahead feature that compares the current match to the next few matches that start at the following bytes. The number of bytes to look ahead depends on the comprssion level. To select the best match the parser calculates the cost of the literals that will be emitted if a further match will be selected, the location of the end of the match, and the offset to the match destination (longer matches take more bytes to encode). 

It is difficult to have an accurate cost model because the cost of the offset field and the literal sequence after the entropy encoder depends on decisions before and after the current match. 

## Optimal Parser

The optimal parser attempts to generate the best possible sequence of matches, considering the limitations of inaccurate cost model. The optimal parser scans the input and generates the list of all possible matches. Next, it scans the list of matches backwards and calculates the best possible distance and path for each byte in the input stream. The dynamic programming algorithm makes the match selection an O(n) algorithm, but the main cost of the scan is the search for matches for each byte in the stream. It is possible to improve the performance of the optimal parser by splitting the input into segments with little loss of correctnes, but this feature is still not implementd. The optimal parser is not always better than the traditional look-ahead parser because of cost-model limitations.  

# Encoder

The buffer that contains all of the literals is split into small chunks (typically 64k). Splitting the regions into smaller sections allow the entropy encoder to have sharper histograms.  This happens when two different parts of the file have a different distribution of values. 

