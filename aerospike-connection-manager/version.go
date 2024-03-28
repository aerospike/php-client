package main

import (
	"regexp"
	"strconv"
)

type versionStatus int

const (
	vsNewer versionStatus = iota
	vsOlder
	vsEqual
)

const pattern = `(?P<v1>\d+)(\.(?P<v2>\d+)(\.(?P<v3>\d+)(\.(?P<v4>\d+))?)?)?.*`

var vregex = regexp.MustCompile(pattern)

func cmpServerVersion(sv, v string) versionStatus {
	server := findNamedMatches(sv)
	req := findNamedMatches(v)

	for i := 0; i < 4; i++ {
		if req[i] < server[i] {
			return vsNewer
		} else if req[i] > server[i] {
			return vsOlder
		}
	}

	return vsEqual
}

func serverIsOlderThan(sv, v string) bool {
	return cmpServerVersion(sv, v) == vsOlder
}

func serverIsNewerThan(sv, v string) bool {
	return cmpServerVersion(sv, v) != vsOlder
}

func findNamedMatches(str string) []int {
	match := vregex.FindStringSubmatch(str)
	names := vregex.SubexpNames()
	results := make([]int, len(names))

	j := 0
	for i, vstr := range match {
		if len(names[i]) > 0 {
			vr, _ := strconv.Atoi(vstr)
			results[j] = vr
			j++
		}
	}
	return results[:j]
}
